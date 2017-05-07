using org.pdfclown.documents;
using org.pdfclown.documents.contents.composition;
using org.pdfclown.documents.contents.entities;
using fonts = org.pdfclown.documents.contents.fonts;
using org.pdfclown.documents.contents.xObjects;
using files = org.pdfclown.files;

using System;
using System.Collections.Generic;
using System.Drawing;
using System.IO;

namespace org.pdfclown.samples.cli
{
  /**
    <summary>This sample demonstrates the PDF Clown's support to Unicode-compliant fonts.</summary>
  */
  public class UnicodeSample
    : Sample
  {
    private const float Margin = 36;

    public override void Run(
      )
    {
      // 1. Instantiate a new PDF file!
      files::File file = new files::File();
      Document document = file.Document;

      // 2. Insert the contents into the document!
      Populate(document);

      // 3. Serialize the PDF file!
      Serialize(file, "Unicode", "using Unicode fonts", "Unicode");
    }

    /**
      <summary>Populates a PDF file with contents.</summary>
    */
    private void Populate(
      Document document
      )
    {
      // 1. Add the page to the document!
      Page page = new Page(document); // Instantiates the page inside the document context.
      document.Pages.Add(page); // Puts the page in the pages collection.

      // 2.1. Create a content composer for the page!
      PrimitiveComposer composer = new PrimitiveComposer(page);

      // 2.2. Create a block composer!
      BlockComposer blockComposer = new BlockComposer(composer);

      // 3. Inserting contents...
      // Define the font to use!
      fonts::Font font = fonts::Font.Get(
        document,
        GetResourcePath("fonts" + Path.DirectorySeparatorChar + "GenR102.TTF")
        );
      // Define the paragraph break size!
      Size breakSize = new Size(0,10);
      // Define the text to show!
      string[] titles = new string[]
        {
          "ΑΡΘΡΟ 1",
          "ASARIYA SINTE (1)",
          "Article 1",
          "Article premier",
          "Статья 1",
          "Artículo 1",
          "Artikel 1",
          "Madde 1",
          "Artikel 1",
          "Articolo 1",
          "Artykuł 1",
          "Bend 1",
          "Abala kìíní."
        };
      string[] bodies = new string[]
        {
          "'Ολοι οι άνθρωποι γεννιούνται ελεύθεροι και ίσοι στην αξιοπρέπεια και τα δικαιώματα. Είναι προικισμένοι με λογική και συνείδηση, και οφείλουν να συμπεριφέρονται μεταξύ τους με πνεύμα αδελφοσύνης.",
          "Aduniya kuna n gu ibuna damayo hɛi nɔ dei-dei nn daama nna n burucinitɛrɛ fɔ, n lasabu nna laakari ya nam nn mɔ huro cɛrɛ kuna nyanze tɛrɛ bɔŋɔɔ.",
          "All human beings are born free and equal in dignity and rights. They are endowed with reason and conscience and should act towards one another in a spirit of brotherhood.",
          "Tous les êtres humains naissent libres et égaux en dignité et en droits. Ils sont doués de raison et de conscience et doivent agir les uns envers les autres dans un esprit de fraternité.",
          "Все люди рождаются свободными и равными в своем достоинстве и правах. Они наделены разумом и совестью и должны поступать в отношении друг друга в духе братства.",
          "Todos los seres humanos nacen libres e iguales en dignidad y derechos y, dotados como están de razón y conciencia, deben comportarse fraternalmente los unos con los otros.",
          "Alle Menschen sind frei und gleich an Würde und Rechten geboren. Sie sind mit Vernunft und Gewissen begabt und sollen einander im Geist der Brüderlichkeit begegnen.",
          "Bütün insanlar hür, haysiyet ve haklar bakımından eşit doğarlar. Akıl ve vicdana sahiptirler ve birbirlerine karşı kardeşlik zihniyeti ile hareket etmelidirler.",
          "Alla människor är födda fria och lika i värde och rättigheter. De har utrustats med förnuft och samvete och bör handla gentemot varandra i en anda av gemenskap.",
          "Tutti gli esseri umani nascono liberi ed eguali in dignità e diritti. Essi sono dotati di ragione e di coscienza e devono agire gli uni verso gli altri in spirito di fratellanza.",
          "Wszyscy ludzie rodzą się wolni i równi pod względem swej godności i swych praw. Są oni obdarzeni rozumem i sumieniem i powinni postępować wobec innych w duchu braterstwa.",
          "Hemû mirov azad û di weqar û mafan de wekhev tên dinyayê. Ew xwedî hiş û şuûr in û divê li hember hev bi zihniyeteke bratiyê bilivin.",
          "Gbogbo ènìyàn ni a bí ní òmìnira; iyì àti è̟tó̟ kò̟ò̟kan sì dó̟gba. Wó̟n ní è̟bùn ti làákàyè àti ti è̟rí-o̟kàn, ó sì ye̟ kí wo̟n ó máa hùwà sí ara wo̟n gé̟gé̟ bí o̟mo̟ ìyá."
        };
      string[] sources = new string[]
      {
        "http://www.ohchr.org/EN/UDHR/Pages/Language.aspx?LangID=grk",
        "http://www.ohchr.org/EN/UDHR/Pages/Language.aspx?LangID=den",
        "http://www.ohchr.org/EN/UDHR/Pages/Language.aspx?LangID=eng",
        "http://www.ohchr.org/EN/UDHR/Pages/Language.aspx?LangID=frn",
        "http://www.ohchr.org/EN/UDHR/Pages/Language.aspx?LangID=rus",
        "http://www.ohchr.org/EN/UDHR/Pages/Language.aspx?LangID=spn",
        "http://www.ohchr.org/EN/UDHR/Pages/Language.aspx?LangID=ger",
        "http://www.ohchr.org/EN/UDHR/Pages/Language.aspx?LangID=trk",
        "http://www.ohchr.org/EN/UDHR/Pages/Language.aspx?LangID=swd",
        "http://www.ohchr.org/EN/UDHR/Pages/Language.aspx?LangID=itn",
        "http://www.ohchr.org/EN/UDHR/Pages/Language.aspx?LangID=pql",
        "http://www.ohchr.org/EN/UDHR/Pages/Language.aspx?LangID=kdb1",
        "http://www.ohchr.org/EN/UDHR/Pages/Language.aspx?LangID=yor"
      };
      // Begin the content block!
      blockComposer.Begin(
        new RectangleF(
          Margin,
          Margin,
          page.Size.Width - Margin * 2,
          page.Size.Height - Margin * 2
          ),
        XAlignmentEnum.Justify,
        YAlignmentEnum.Top
        );
      for(
        int index = 0,
          length = titles.Length;
        index < length;
        index++
        )
      {
        composer.SetFont(font,12);
        blockComposer.ShowText(titles[index]);
        blockComposer.ShowBreak();

        composer.SetFont(font,11);
        blockComposer.ShowText(bodies[index]);
        blockComposer.ShowBreak(XAlignmentEnum.Right);

        composer.SetFont(font,8);
        blockComposer.ShowText("[Source: " + sources[index] + "]");
        blockComposer.ShowBreak(breakSize,XAlignmentEnum.Justify);
      }
      // End the content block!
      blockComposer.End();

      // 4. Flush the contents into the page!
      composer.Flush();
    }
  }
}

